class Comment
  belongs_to :post, class_name: 'SpecialPost', foreign_key: 'post_id', dependent: :destroy
                                               ^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
