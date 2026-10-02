belongs_to :author, foreign_key: :user_id, optional: true
validates :author, presence: true
