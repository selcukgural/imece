import unittest

from shelf import Book, Shelf


def three():
    s = Shelf()
    s.add(Book("Dune", "Frank Herbert", 1965))
    s.add(Book("Emma", "Jane Austen", 1815))
    s.add(Book("Beloved", "Toni Morrison", 1987))
    return s


class ShelfTest(unittest.TestCase):
    def test_add_twice_is_refused(self):
        s = three()
        with self.assertRaises(ValueError):
            s.add(Book("Dune", "someone", 2000))

    def test_find_by_title(self):
        self.assertEqual([b.title for b in three().find("Em")], ["Emma"])

    def test_listing_is_by_title(self):
        self.assertEqual(
            [b.title for b in three().listing()], ["Beloved", "Dune", "Emma"]
        )

    def test_len(self):
        self.assertEqual(len(three()), 3)


if __name__ == "__main__":
    unittest.main()
