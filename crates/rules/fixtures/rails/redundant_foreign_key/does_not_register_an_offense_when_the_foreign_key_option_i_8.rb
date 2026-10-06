class Book
  has_many :chapter, as: :publishable, foreign_key: 'book_id'
end
