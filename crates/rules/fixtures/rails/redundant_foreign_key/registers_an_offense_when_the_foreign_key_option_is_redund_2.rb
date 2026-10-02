class Book
  has_one :chapter, foreign_key: 'book_id'
                    ^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
