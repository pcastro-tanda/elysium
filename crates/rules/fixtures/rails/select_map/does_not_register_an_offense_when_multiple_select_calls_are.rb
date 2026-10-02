(Foo.select(:column_name) + Bar.select(:column_name)).map(&:column_name)
