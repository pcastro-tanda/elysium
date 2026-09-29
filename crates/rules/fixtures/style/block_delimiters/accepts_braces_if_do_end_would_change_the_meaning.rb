scope :foo, lambda { |f|
  where(condition: "value")
}

expect { something }.to raise_error(ErrorClass) { |error|
  # ...
}

expect { x }.to change {
  Counter.count
}.from(0).to(1)

cr.stubs client: mock {
  expects(:email_disabled=).with(true)
  expects :save
}
