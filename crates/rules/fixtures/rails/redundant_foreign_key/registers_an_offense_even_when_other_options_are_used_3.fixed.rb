class Book
  has_many :chapter, class_name: 'SpecialChapter', dependent: :destroy
end
