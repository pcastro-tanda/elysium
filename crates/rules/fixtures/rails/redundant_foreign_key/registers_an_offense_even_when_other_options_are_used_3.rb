class Book
  has_many :chapter, class_name: 'SpecialChapter', foreign_key: 'book_id', dependent: :destroy
                                                   ^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
