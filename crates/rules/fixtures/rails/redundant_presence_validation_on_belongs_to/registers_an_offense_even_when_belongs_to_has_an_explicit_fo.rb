belongs_to :author, foreign_key: :user_id
validates :author, presence: true
                   ^^^^^^^^^^^^^^ Remove explicit presence validation for `author`.
