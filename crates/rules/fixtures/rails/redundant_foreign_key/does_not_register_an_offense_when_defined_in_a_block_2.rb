class_methods do
  has_many :chapter, foreign_key: 'book_id'
end
