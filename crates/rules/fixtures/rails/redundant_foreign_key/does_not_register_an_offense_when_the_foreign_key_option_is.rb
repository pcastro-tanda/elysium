class Comment
  belongs_to :author, foreign_key: 'user_id'
end
