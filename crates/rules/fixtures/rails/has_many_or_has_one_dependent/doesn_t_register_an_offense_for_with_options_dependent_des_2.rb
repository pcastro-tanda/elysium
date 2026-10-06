class Person < ApplicationRecord
  with_options dependent: :destroy do
    has_many :foo do
      def bar
      end
    end
  end
end
