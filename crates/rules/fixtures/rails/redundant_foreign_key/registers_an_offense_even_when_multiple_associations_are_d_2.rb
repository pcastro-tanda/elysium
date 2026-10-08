class Book
  belongs_to :series

  has_many :chapter, foreign_key: 'book_id'
                     ^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
