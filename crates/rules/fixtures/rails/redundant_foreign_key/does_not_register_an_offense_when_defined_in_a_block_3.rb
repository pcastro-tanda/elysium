class_methods do
  has_and_belongs_to_many :chapter, foreign_key: 'book_id'
end
