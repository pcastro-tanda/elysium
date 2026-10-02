travel_to(Time.now)
^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(Time.new)
^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(DateTime.now)
^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(Time.current)
^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(Time.zone.now)
^^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(Time.now.in_time_zone)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(Time.current.to_time)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(::Time.now)
^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(::DateTime.now)
^^^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
travel_to(::Time.zone.now)
^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
