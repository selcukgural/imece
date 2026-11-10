"""shelf's command line: `python3 cli.py list` and `python3 cli.py find TEXT`."""

import sys

from shelf import Book, Shelf


def sample():
    s = Shelf()
    s.add(Book("Dune", "Frank Herbert", 1965))
    s.add(Book("Emma", "Jane Austen", 1815))
    return s


def main(argv):
    s = sample()
    if argv[:1] == ["list"]:
        books = s.listing()
    elif argv[:1] == ["find"] and len(argv) == 2:
        books = s.find(argv[1])
    else:
        print("usage: cli.py list | find TEXT", file=sys.stderr)
        return 2
    for b in books:
        print(f"{b.title} · {b.author} · {b.year}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
