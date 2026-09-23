class MyModel < ActiveRecord::Base
  has_many :other_models,
    class_name: "legacy_name",
    ^^^^^^^^^^^^^^^^^^^^^^^^^^ Align the arguments of a method call if they span more than one line.
    order: "#{legacy_name.table_name}.published DESC"

end
