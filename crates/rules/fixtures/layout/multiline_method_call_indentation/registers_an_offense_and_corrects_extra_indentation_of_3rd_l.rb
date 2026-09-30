expect { Foo.new }.
  to change { Bar.count }.
      from(1).to(2)
      ^^^^ Use 2 (not 6) spaces for indenting an expression spanning multiple lines.
