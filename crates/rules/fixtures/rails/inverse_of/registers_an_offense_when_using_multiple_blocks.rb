class Book < ApplicationRecord
  with_options conditions: -> () { where(famous: true) } do
    with_helper do |helper|
      helper.define_assoc
      with_options foreign_key: "patron_id" do
        belongs_to :author
        ^^^^^^^^^^ Specify an `:inverse_of` option.
      end
    end
  end
end
