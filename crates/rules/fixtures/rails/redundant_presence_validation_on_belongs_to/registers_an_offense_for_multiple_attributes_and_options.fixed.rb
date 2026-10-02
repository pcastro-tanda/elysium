belongs_to :user
validates :name, presence: true, uniqueness: true
validates :user, uniqueness: true
