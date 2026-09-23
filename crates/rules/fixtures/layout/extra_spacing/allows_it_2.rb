size_attribute_name    = FactoryGirl.create(:attribute,
                                            name:   'Size',
                                            values: %w{small large})
carrier_attribute_name = FactoryGirl.create(:attribute,
                                            name:   'Carrier',
                                            values: %w{verizon})
