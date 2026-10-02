belongs_to :user # belongs_to is optional by default in Rails 4.2
validates :user, presence: true
