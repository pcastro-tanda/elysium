class Book < ApplicationRecord
  with_options inverse_of: :book do
    with_helper do |helper|
      helper.define_assoc
      with_options foreign_key: 'book_id' do
        belongs_to :author
      end
    end
  end
end
