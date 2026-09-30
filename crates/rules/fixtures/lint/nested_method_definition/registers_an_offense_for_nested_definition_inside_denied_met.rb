def do_something
  denied_method :articles do
    def find_or_create_by_name(name)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Method definitions must not be nested. Use `lambda` instead.
    end
  end
end
