use crate::{DerivesRow, Opcode, Operand, Plan, PlanError};

impl Plan {
    /// The body and owner behind an at-read derive load; settled loads stay values.
    /// Static readers use this accessor before interpreting a load as an opaque value.
    pub fn unsettled_body(&self, index: u32) -> Option<&DerivesRow> {
        self.derives.get(index as usize).filter(|row| row.unsettled)
    }

    pub(crate) fn validate_unsettled(&self) -> Result<(), PlanError> {
        for (i, row) in self.derives.iter().enumerate() {
            if !row.unsettled {
                if row.owner.is_some() {
                    return Err(PlanError::BadReference {
                        table: "derives",
                        row: i as u32,
                        field: "owner",
                    });
                }
                continue;
            }
            let mut reader = crate::bytes::Reader::new(self.code(row.body));
            while !reader.is_empty() {
                let op = Opcode::from_wire(reader.u8()?).expect("validated opcode");
                for operand in op.operands() {
                    match operand {
                        Operand::U8 | Operand::Enum(_) => {
                            reader.u8()?;
                        }
                        Operand::U16 => {
                            reader.u16()?;
                        }
                        Operand::U32 | Operand::Str | Operand::Idx(_) => {
                            let n = reader.u32()?;
                            if op == Opcode::LoadDerive
                                && self.unsettled_body(n).is_some()
                                && n as usize >= i
                            {
                                return Err(PlanError::DeriveOrder {
                                    row: i as u32,
                                    dependency: n,
                                });
                            }
                        }
                        Operand::F64 => {
                            reader.f64()?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{asm::Asm, builder::PlanBuilder, DerivesId, PlanError, TypeKind, Value};

    #[test]
    fn executable_rows_refuse_self_and_forward_calls() {
        for dependency in [0, 1] {
            let mut b = PlanBuilder::new(0, 1);
            let ty = b.primitive(TypeKind::Number);
            let mut a = Asm::default();
            a.load_derive(DerivesId(dependency));
            let code = b.code(a);
            let first = b.derive("first", ty, code);
            b.set_derive_unsettled(first, None);
            let code = b.constant(&Value::Number(1.0));
            let second = b.derive("second", ty, code);
            b.set_derive_unsettled(second, None);
            assert!(matches!(
                b.finish(),
                Err(PlanError::DeriveOrder { row: 0, .. })
            ));
        }
    }
}
