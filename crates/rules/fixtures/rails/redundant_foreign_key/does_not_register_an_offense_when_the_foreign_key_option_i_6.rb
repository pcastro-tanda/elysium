class Book
  has_many :chapter, foreign_key: 'publication_id'
end
