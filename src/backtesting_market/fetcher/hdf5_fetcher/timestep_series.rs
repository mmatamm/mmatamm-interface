use std::{collections::BTreeMap, ops::Bound};

use chrono::NaiveDateTime;

// TODO Use derive types
#[derive(Debug)]
pub(super) struct TimestepSeries<T> {
    data: Vec<(i64, T)>,
    days_map: BTreeMap<u16, usize>,
}

impl<T> TimestepSeries<T> {
    pub fn new(data: Vec<(i64, T)>) -> Self {
        let days_map = Self::map_days(&data);

        TimestepSeries { data, days_map }
    }

    fn map_days(data: &Vec<(i64, T)>) -> BTreeMap<u16, usize> {
        let mut map = BTreeMap::new();

        let mut prev_date: Option<u16> = None;

        for (i, &(timestamp, _)) in data.iter().enumerate() {
            let date = timestamp_to_date(timestamp);

            // If this is the first occurrence of this date, insert it
            if prev_date.as_ref() != Some(&date) {
                map.insert(date, i);
                prev_date = Some(date);
            }
        }

        map
    }

    /// Queries the time series for a value at or around the specified
    /// timestamp.
    ///
    /// If the specified date does not exist in the dataset, this function will
    /// attempt to return the first value of the next day (if direction is
    /// `Forward`) or the last value of the previous day (if direction is
    /// `Backward`).
    ///
    /// # Arguments
    ///
    /// * `time`: The timestamp to query.
    /// * `direction`: The direction in which to search.
    /// * `bound`: Whether to include or exclude the exact timestamp match.
    ///
    /// # Returns
    ///
    /// The row of data at the found timestamp, or None if not found.
    pub fn query(
        &self,
        time: &NaiveDateTime,
        direction: QueryDirection,
        bound: TimeQueryBound,
    ) -> Option<&(i64, T)> {
        let timestamp = time.and_utc().timestamp();
        let date = timestamp_to_date(timestamp);

        // Try to find the index for the specified date.
        match self.days_map.get(&date).copied() {
            Some(date_index) => {
                // Date exists, perform query within its range.
                self.query_in_range(timestamp, date, direction, bound, date_index)
            }
            None => {
                // Date does not exist, fallback to the nearest day.
                self.query_nearest_day(date, direction)
            }
        }
    }

    /// Convenience method to get the next value after the specified time
    /// Returns None if no such value exists, or an HDF5 error if there is a
    /// problem accessing the data.
    pub fn query_after(&self, time: &NaiveDateTime) -> Option<&(i64, T)> {
        self.query(time, QueryDirection::Forward, TimeQueryBound::Exclusive)
    }

    /// Convenience method to get the last value before or at the specified
    /// time. Returns None if no such value exists, or an HDF5 error if
    /// there is a problem accessing the data.
    pub fn query_before(&self, time: &NaiveDateTime) -> Option<&(i64, T)> {
        self.query(time, QueryDirection::Backward, TimeQueryBound::Inclusive)
    }

    /// Helper function to query within a known date range.
    fn query_in_range(
        &self,
        timestamp: i64,
        date: u16,
        direction: QueryDirection,
        bound: TimeQueryBound,
        date_index: usize,
    ) -> Option<&(i64, T)> {
        let next_day_index = self
            .days_map
            .lower_bound(Bound::Excluded(&date))
            .next()
            .map(|(_, &i)| i)
            .unwrap_or(self.data.len());

        let start_index = date_index.saturating_sub(1);

        // TODO Is this min necessary?
        let end_index = (next_day_index + 1).min(self.data.len());

        let search_range = &self.data[start_index..end_index];

        let row_index = match (direction, bound) {
            (QueryDirection::Forward, TimeQueryBound::Inclusive) => {
                search_range.partition_point(|(ts, _)| ts < &timestamp)
            }
            (QueryDirection::Forward, TimeQueryBound::Exclusive) => {
                search_range.partition_point(|(ts, _)| ts <= &timestamp)
            }
            (QueryDirection::Backward, TimeQueryBound::Inclusive) => search_range
                .partition_point(|(ts, _)| ts <= &timestamp)
                .saturating_sub(1),
            (QueryDirection::Backward, TimeQueryBound::Exclusive) => search_range
                .partition_point(|(ts, _)| ts < &timestamp)
                .saturating_sub(1),
        };

        if row_index >= search_range.len() {
            return None;
        }

        Some(&search_range[row_index])
    }

    /// Helper function to handle cases where the specified date does not exist.
    fn query_nearest_day(&self, date: u16, direction: QueryDirection) -> Option<&(i64, T)> {
        if let Some((_, next_day_index)) = self.days_map.lower_bound(Bound::Excluded(&date)).next()
        {
            match direction {
                QueryDirection::Backward => {
                    // Fallback to last row of previous day
                    if *next_day_index > 0 {
                        Some(&self.data[next_day_index - 1])
                    } else {
                        None
                    }
                }
                QueryDirection::Forward => {
                    // Fallback to first row of next day
                    Some(&self.data[*next_day_index])
                }
            }
        } else {
            if let QueryDirection::Backward = direction {
                self.data.last()
            } else {
                None
            }
        }
    }
}

/// Represents the bound type for time series queries
#[derive(Debug, Clone, Copy)]
pub enum TimeQueryBound {
    /// Include the exact timestamp if it exists
    Inclusive,
    /// Exclude the exact timestamp
    Exclusive,
}

/// Direction for querying in the time series
#[derive(Debug, Clone, Copy)]
pub enum QueryDirection {
    /// Search forward in time
    Forward,
    /// Search backward in time
    Backward,
}

const SECONDS_IN_DAY: i32 = 60 * 60 * 24;

fn timestamp_to_date(timestamp: i64) -> u16 {
    (timestamp / SECONDS_IN_DAY as i64) as u16
}
