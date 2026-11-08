"""Commands that talk to the running app: list, add, remove, say, steal, quit.

Each sends one request over the control socket (see ipc.py) and prints the
outcome. They exit non-zero if the app isn't running or nothing matched, so
they work in scripts.
"""
from __future__ import annotations

import argparse
import json
import sys

TARGET_HELP = "a mascot id (see 'list') or name"


def _request(message: dict) -> dict:
    """Send one request; exit with an error if the app isn't running or refuses."""
    from PySide6.QtCore import QCoreApplication

    from .ipc import send

    _app = QCoreApplication.instance() or QCoreApplication(sys.argv[:1])  # noqa: F841 -- QLocalSocket needs one
    reply = send(message)
    if reply is None:
        sys.exit("desktop-mascots isn't running (start it with: desktop-mascots)")
    if not reply.get("ok"):
        sys.exit(f"error: {reply.get('error')}")
    return reply


def _check_target(parser: argparse.ArgumentParser, target: str, everyone: str) -> None:
    from .characters import CHARACTERS
    if target != everyone and not target.isdigit() and target not in CHARACTERS:
        parser.error(f"unknown mascot: {target} (use {TARGET_HELP}, or {everyone})")


def _read_text(words: list[str]) -> str:
    text = sys.stdin.read() if words == ["-"] else " ".join(words)
    return " ".join(text.split())   # one paragraph; the bubble wraps it


def _label(m: dict) -> str:
    return f"{m['name']} #{m['id']}"


# ----------------------------------------------------------------- commands
def list_command(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="desktop-mascots list",
                                     description="Show the mascots that are on screen.")
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    args = parser.parse_args(argv)
    mascots = _request({"cmd": "list"}).get("mascots", [])
    if args.json:
        print(json.dumps(mascots, indent=2))
        return 0
    if not mascots:
        print("No mascots on screen. Add some with: desktop-mascots add MASCOT")
        return 0
    print(f"{'ID':>3}  {'MASCOT':<9} {'DOING':<9} {'WHERE':<13} NOTES")
    for m in mascots:
        notes = []
        if m.get("leaves_in") is not None:
            notes.append(f"leaves in {m['leaves_in']:.0f}s")
        if m.get("holding_pointer"):
            notes.append("holding your pointer")
        print(f"{m['id']:>3}  {m['name']:<9} {m['state']:<9} {m['x']:>5},{m['y']:<7} {', '.join(notes)}")
    return 0


def add_command(argv: list[str]) -> int:
    from .characters import CHARACTERS

    parser = argparse.ArgumentParser(prog="desktop-mascots add",
                                     description="Add mascots to the running app.")
    parser.add_argument("mascots", nargs="+", metavar="MASCOT",
                        help=f"what to add, repeat for more ({', '.join(CHARACTERS)})")
    parser.add_argument("--time", type=float, metavar="SECONDS", help="they leave after this many seconds")
    parser.add_argument("--say", metavar="TEXT", help="their opening line ('-' reads it from stdin)")
    args = parser.parse_args(argv)
    unknown = [m for m in args.mascots if m not in CHARACTERS]
    if unknown:
        parser.error(f"unknown mascot(s): {', '.join(unknown)} (available: {', '.join(CHARACTERS)})")
    if args.time is not None and args.time <= 0:
        parser.error("--time must be greater than 0")
    say = _read_text([args.say]) if args.say else None
    reply = _request({"cmd": "spawn", "mascots": args.mascots, "lifetime": args.time, "say": say})
    print("added " + (", ".join(_label(m) for m in reply.get("mascots", [])) or ", ".join(args.mascots)))
    return 0


def remove_command(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="desktop-mascots remove",
        description="Remove mascots: by id (see 'list'), every one of a kind by name, or --all.")
    parser.add_argument("targets", nargs="*", metavar="TARGET", help=TARGET_HELP)
    parser.add_argument("--all", action="store_true", help="remove every mascot (the app keeps running)")
    args = parser.parse_args(argv)
    if not args.targets and not args.all:
        parser.error("say which mascots to remove (an id or name), or --all")
    for t in args.targets:
        _check_target(parser, t, "all")
    reply = _request({"cmd": "remove", "targets": ["all"] if args.all else args.targets})
    if not reply.get("mascots"):
        sys.exit(f"no mascot matched: {' '.join(args.targets)}")
    print("removed " + ", ".join(_label(m) for m in reply["mascots"]))
    return 0


def say_command(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="desktop-mascots say",
                                     description="Make the running mascots say something.")
    parser.add_argument("text", nargs="+", help="what to say ('-' reads it from stdin)")
    parser.add_argument("--to", default="all", metavar="TARGET", help=f"who says it: all (default), {TARGET_HELP}")
    parser.add_argument("--seconds", type=float, help="how long the bubble stays (default: based on length)")
    args = parser.parse_args(argv)
    _check_target(parser, args.to, "all")
    text = _read_text(args.text)
    if not text:
        parser.error("nothing to say")
    reply = _request({"cmd": "say", "text": text, "to": args.to, "seconds": args.seconds})
    if reply.get("count", 0) == 0:
        sys.exit(f"no {args.to} on screen right now")
    return 0


def steal_command(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="desktop-mascots steal",
        description="A mascot runs to your pointer, grabs it and runs off with it. "
                    "Click the thief to get it back early.")
    parser.add_argument("--to", default="any", metavar="TARGET",
                        help=f"who steals it: any (default, picked at random), {TARGET_HELP}")
    parser.add_argument("--seconds", type=float, default=5.0,
                        help="how long it holds on to the pointer (default 5, max 60)")
    args = parser.parse_args(argv)
    _check_target(parser, args.to, "any")
    if not 0 < args.seconds <= 60:
        parser.error("--seconds must be more than 0 and at most 60")
    _request({"cmd": "steal", "to": args.to, "seconds": args.seconds})
    return 0


def quit_command(argv: list[str]) -> int:
    argparse.ArgumentParser(prog="desktop-mascots quit",
                            description="Close the running mascots.").parse_args(argv)
    _request({"cmd": "quit"})
    return 0


COMMANDS = {
    "list": list_command,
    "add": add_command,
    "remove": remove_command,
    "say": say_command,
    "steal": steal_command,
    "quit": quit_command,
}
