class Book
  has_one :chapter, class_name: 'SpecialChapter', dependent: :destroy
end
