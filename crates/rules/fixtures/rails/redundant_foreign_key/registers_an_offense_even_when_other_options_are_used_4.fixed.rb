class Book
  has_and_belongs_to_many :chapter, class_name: 'SpecialChapter', dependent: :destroy
end
