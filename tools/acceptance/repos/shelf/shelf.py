"""A small library shelf: books, adding, finding and listing them."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Book:
    title: str
    author: str
    year: int


class Shelf:
    def __init__(self):
        self._books = {}

    def add(self, book):
        if book.title in self._books:
            raise ValueError(f"{book.title!r} is on the shelf already")
        self._books[book.title] = book

    def remove(self, title):
        del self._books[title]

    def find(self, text):
        """The books whose title holds `text`."""
        return [b for b in self._books.values() if text in b.title]

    def listing(self):
        """Every book, by title."""
        return sorted(self._books.values(), key=lambda b: b.title)

    def __len__(self):
        return len(self._books)
