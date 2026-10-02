class Book
  has_and_belongs_to_many :chapter, foreign_key: 'book_id'
                                    ^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
