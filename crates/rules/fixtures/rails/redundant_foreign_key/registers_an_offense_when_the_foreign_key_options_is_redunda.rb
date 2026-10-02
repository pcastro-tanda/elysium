class Book
  has_one :chapter, as: :publishable, foreign_key: 'publishable_id'
                                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
