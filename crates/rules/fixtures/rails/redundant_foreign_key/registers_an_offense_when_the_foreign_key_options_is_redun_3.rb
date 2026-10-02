class Book
  has_and_belongs_to_many :chapter, as: :publishable, foreign_key: 'publishable_id'
                                                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
