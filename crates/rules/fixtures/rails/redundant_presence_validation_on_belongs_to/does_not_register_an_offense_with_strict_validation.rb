belongs_to :user
validates :user, presence: true, strict: true
