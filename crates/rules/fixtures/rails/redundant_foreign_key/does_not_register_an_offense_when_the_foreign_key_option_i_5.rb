class Book
  has_one :chapter, as: :publishable, foreign_key: 'book_id'
end
