expect { Foo.new }.
  to change { Bar.count }.
       from(1).to(2)
