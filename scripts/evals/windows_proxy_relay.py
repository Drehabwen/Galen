"""Expose a Windows loopback proxy to WSL without changing global proxy settings."""

from __future__ import annotations

import argparse
import socket
import socketserver
import threading


class ProxyRelayHandler(socketserver.BaseRequestHandler):
    def handle(self) -> None:
        upstream = socket.create_connection(
            (self.server.target_host, self.server.target_port), timeout=15
        )

        def pump(source: socket.socket, destination: socket.socket) -> None:
            try:
                while data := source.recv(65536):
                    destination.sendall(data)
            except (ConnectionError, OSError):
                pass
            finally:
                try:
                    destination.shutdown(socket.SHUT_WR)
                except OSError:
                    pass

        try:
            self.request.settimeout(None)
            upstream.settimeout(None)
            request_pump = threading.Thread(
                target=pump,
                args=(self.request, upstream),
                daemon=True,
            )
            request_pump.start()
            pump(upstream, self.request)
            request_pump.join(timeout=5)
        finally:
            upstream.close()


class ThreadingProxyRelay(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(
        self,
        listen: tuple[str, int],
        target: tuple[str, int],
    ) -> None:
        self.target_host, self.target_port = target
        super().__init__(listen, ProxyRelayHandler)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--listen-host", default="0.0.0.0")
    parser.add_argument("--listen-port", type=int, default=17890)
    parser.add_argument("--target-host", default="127.0.0.1")
    parser.add_argument("--target-port", type=int, default=7890)
    args = parser.parse_args()

    with ThreadingProxyRelay(
        (args.listen_host, args.listen_port),
        (args.target_host, args.target_port),
    ) as server:
        print(
            f"Relaying {args.listen_host}:{args.listen_port} to "
            f"{args.target_host}:{args.target_port}",
            flush=True,
        )
        server.serve_forever()


if __name__ == "__main__":
    main()
