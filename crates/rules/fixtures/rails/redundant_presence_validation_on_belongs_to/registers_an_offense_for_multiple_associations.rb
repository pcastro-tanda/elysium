belongs_to :user
belongs_to :book
validates :user, :book, presence: true
                        ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`/`book`.
