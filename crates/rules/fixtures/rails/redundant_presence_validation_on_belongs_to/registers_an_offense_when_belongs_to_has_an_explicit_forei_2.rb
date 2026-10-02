belongs_to :author, class_name: 'User', foreign_key: :user_id
validates :user_id, presence: true
                    ^^^^^^^^^^^^^^ Remove explicit presence validation for `user_id`.
