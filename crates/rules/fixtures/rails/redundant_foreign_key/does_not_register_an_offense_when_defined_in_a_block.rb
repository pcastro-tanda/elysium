class_methods do
  has_one :chapter, foreign_key: 'book_id'
end
