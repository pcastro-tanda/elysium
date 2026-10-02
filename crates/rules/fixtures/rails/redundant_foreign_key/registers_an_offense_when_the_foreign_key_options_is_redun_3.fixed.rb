class Book
  has_and_belongs_to_many :chapter, as: :publishable
end
