module User
  extend ActiveSupport::Concern
  included do
    validates :account, uniqueness: true
  end
end
