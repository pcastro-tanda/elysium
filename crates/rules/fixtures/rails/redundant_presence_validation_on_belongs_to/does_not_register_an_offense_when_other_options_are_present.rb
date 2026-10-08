belongs_to :user
validates :user, presence: true, allow_blank: false
