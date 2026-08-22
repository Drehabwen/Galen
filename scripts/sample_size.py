"""样本量计算：12 周有氧运动 vs 常规康复 RCT，主要终点 6MWT 变化(m)
依据：既往 RCT (Macko 2005 PMID:16151035; Globas 2012 PMID:21885867) 与
荟萃分析 (Mehta 2012 PMID:23192710; Moncion 2024 PMID:38413134)
参数：alpha=0.05(双侧) beta=0.20(功效80%) 失访15%
公式：n/组 = 2*(Z_{1-a/2}+Z_{1-b})^2 * sigma^2 / delta^2
"""
import math
alpha, beta = 0.05, 0.20
z_alpha, z_beta = 1.959964, 0.8416212
attrition = 0.15
def n_per_group(delta, sigma):
    return math.ceil(2 * (z_alpha + z_beta)**2 * sigma**2 / delta**2)

if __name__ == "__main__":
    print("主方案: delta=35, sigma=60")
    n0 = n_per_group(35, 60)
    na = math.ceil(n0 / (1 - attrition))
    print(f"无失访: 每组 {n0} 例, 共 {2*n0} 例; 含15%失访: 每组 {na} 例, 共 {2*na} 例")
    print("敏感性分析:")
    for d, s in [(30,60),(40,60),(30,65),(35,65),(40,65),(35,55),(40,55)]:
        n1 = n_per_group(d, s); n2 = math.ceil(n1/(1-attrition))
        print(f"  delta={d}, sigma={s}: 每组{n1} -> 含失访每组{n2}, 共{2*n2}")
