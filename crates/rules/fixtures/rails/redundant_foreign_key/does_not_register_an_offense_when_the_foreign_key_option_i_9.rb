class Book
  has_and_belongs_to_many :chapter, foreign_key: 'publication_id'
end
