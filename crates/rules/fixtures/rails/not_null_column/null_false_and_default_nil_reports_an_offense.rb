add_column :users, :name, :string, null: false, default: nil
                                   ^^^^^^^^^^^ Do not add a NOT NULL column without a default value.
