class Book
  has_one :chapter, foreign_key: 'publication_id'
end
