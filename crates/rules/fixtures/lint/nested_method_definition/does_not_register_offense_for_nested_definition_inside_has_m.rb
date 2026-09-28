def do_something
  has_many :articles do
    def find_or_create_by_name(name)
    end
  end
end
