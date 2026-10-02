class Comment
  belongs_to :post, class_name: 'SpecialPost', dependent: :destroy
end
