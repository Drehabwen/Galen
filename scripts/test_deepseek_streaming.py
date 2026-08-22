"""
DeepSeek V4 streaming test — verify reasoning_content is NOT duplicated.
Tests: simple QA + complex research prompt. Checks separation + coverage.
"""
import requests
import json
import os
import sys

API_KEY = os.environ.get("DEEPSEEK_API_KEY")
BASE_URL = "https://api.deepseek.com/v1"
MODEL = "deepseek-v4-pro"

TEST_CASES = [
    {
        "label": "简单问答",
        "messages": [{"role": "user", "content": "1+1等于几？用一句话回答。"}],
        "max_tokens": 1024,
    },
    {
        "label": "PubMed引用",
        "messages": [{"role": "user", "content": "请用 APA 格式引用 PMID 32561234，并解释这篇论文的主要发现。"}],
        "max_tokens": 8192,
    },
]


def run_test(label, messages, max_tokens):
    resp = requests.post(
        f"{BASE_URL}/chat/completions",
        headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
        json={"model": MODEL, "messages": messages, "stream": True, "max_tokens": max_tokens},
        stream=True,
        timeout=120,
    )
    if resp.status_code != 200:
        return {"error": f"HTTP {resp.status_code}: {resp.text[:300]}"}

    text_chunks = []
    reasoning_chunks = []
    total_events = 0

    for line in resp.iter_lines(decode_unicode=True):
        if not line or not line.startswith("data: "):
            continue
        data_str = line[6:]
        if data_str == "[DONE]":
            break
        total_events += 1
        try:
            chunk = json.loads(data_str)
        except json.JSONDecodeError:
            continue
        for choice in chunk.get("choices", []):
            delta = choice.get("delta", {})
            c = delta.get("content", "")
            r = delta.get("reasoning_content", "")
            if c:
                text_chunks.append(c)
            if r:
                reasoning_chunks.append(r)

    full_text = "".join(text_chunks)
    full_reasoning = "".join(reasoning_chunks)
    leaked = bool(full_reasoning and full_reasoning in full_text)

    return {
        "label": label,
        "events": total_events,
        "text_chunks": len(text_chunks),
        "reasoning_chunks": len(reasoning_chunks),
        "text_len": len(full_text),
        "reasoning_len": len(full_reasoning),
        "leaked": leaked,
        "text_sample": full_text[:200],
        "reasoning_sample": full_reasoning[:200],
    }


all_pass = True
if not API_KEY:
    print("DEEPSEEK_API_KEY is required", file=sys.stderr)
    sys.exit(2)

for tc in TEST_CASES:
    print(f"\n{'=' * 60}")
    print(f"TEST: {tc['label']} (max_tokens={tc['max_tokens']})")
    print(f"{'=' * 60}")

    r = run_test(tc["label"], tc["messages"], tc["max_tokens"])

    if "error" in r:
        print(f"❌ ERROR: {r['error']}")
        all_pass = False
        continue

    print(f"  Events: {r['events']}")
    print(f"  Text chunks: {r['text_chunks']}, Reasoning chunks: {r['reasoning_chunks']}")
    print(f"  Text len: {r['text_len']}, Reasoning len: {r['reasoning_len']}")

    if r["leaked"]:
        print(f"  ❌ FAIL: reasoning LEAKED into text")
        all_pass = False
    else:
        print(f"  ✅ PASS: separate channels")

    if r["text_len"] == 0 and r["reasoning_len"] > 0:
        print(f"  ⚠️  WARN: model ONLY produced reasoning, NO visible answer!")
        print(f"  → 用户只会看到思考框（可展开），没有正文回复")
        all_pass = False
    elif r["text_len"] > 0 and r["reasoning_len"] > 0:
        print(f"  ✅ OK: both reasoning and text present")

    if r["reasoning_sample"]:
        print(f"\n  Reasoning: {r['reasoning_sample'][:150]}...")
    if r["text_sample"]:
        print(f"\n  Text: {r['text_sample'][:150]}...")

print(f"\n{'=' * 60}")
print("VERDICT: " + ("✅ ALL PASS" if all_pass else "❌ ISSUES FOUND"))
print(f"{'=' * 60}")
sys.exit(0 if all_pass else 1)
