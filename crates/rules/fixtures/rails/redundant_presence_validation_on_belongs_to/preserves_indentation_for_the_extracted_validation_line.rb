class Profile
  belongs_to :user
  validates :user, :name, presence: true, uniqueness: true
                          ^^^^^^^^^^^^^^ Remove explicit presence validation for `user`.
end
