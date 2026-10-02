belongs_to :author, foreign_key: :user_id
validates :author_id, presence: true
