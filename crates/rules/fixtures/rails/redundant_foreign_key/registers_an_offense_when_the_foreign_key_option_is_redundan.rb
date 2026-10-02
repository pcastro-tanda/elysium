class Comment
  belongs_to :post, foreign_key: 'post_id'
                    ^^^^^^^^^^^^^^^^^^^^^^ Specifying the default value for `foreign_key` is redundant.
end
