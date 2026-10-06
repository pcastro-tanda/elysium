class Book
  belongs_to :series

  has_and_belongs_to_many :chapter
end
