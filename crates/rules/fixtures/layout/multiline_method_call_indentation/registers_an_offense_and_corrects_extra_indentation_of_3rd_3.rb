expect { Foo.new }.
  to change { Bar.count }.
      from(1).to(2)
      ^^^^ Indent `from` 2 spaces more than `change { Bar.count }` on line 2.
