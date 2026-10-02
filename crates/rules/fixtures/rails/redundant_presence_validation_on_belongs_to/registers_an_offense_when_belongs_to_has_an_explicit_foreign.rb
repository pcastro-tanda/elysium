belongs_to :author, foreign_key: :user_id
validates :user_id, presence: true
                    ^^^^^^^^^^^^^^ Remove explicit presence validation for `user_id`.
