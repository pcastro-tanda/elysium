class Book
  has_one :chapter, class_name: 'SpecialChapter', foreign_key: 'book_id', dependent: :destroy
                                                  ^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
