use std::{collections::BTreeMap, ops::Bound};

use chrono::{DateTime, NaiveDate, NaiveDateTime};
use ndarray::{s, Array2, ArrayView1};

// TODO Use derive types
#[derive(Debug)]
pub(super) struct TimestepSeries {
    data: Array2<i32>,
    timestamp_factor: f32,
    days_map: BTreeMap<NaiveDate, usize>,
}

impl TimestepSeries {
    pub fn new(dataset: hdf5::Dataset, timestamp_factor: f32) -> Result<Self, hdf5::Error> {
        let data: Array2<i32> = dataset.read()?;

        let days_map = Self::map_days(&data.column(0), timestamp_factor);

        Ok(TimestepSeries {
            data,
            timestamp_factor,
            days_map,
        })
    }

    fn map_days(timestamps: &ArrayView1<i32>, timestamp_factor: f32) -> BTreeMap<NaiveDate, usize> {
        let mut map = BTreeMap::new();

        let mut prev_date: Option<NaiveDate> = None;

        for (i, &ts) in timestamps.iter().enumerate() {
            // Convert timestamp to NaiveDate
            let date = Self::timestamp_to_time(ts, timestamp_factor).date();

            // If this is the first occurrence of this date, insert it
            if prev_date.as_ref() != Some(&date) {
                map.insert(date, i);
                prev_date = Some(date);
            }
        }

        map
    }

    fn timestamp_to_time(timestamp: i32, timestamp_factor: f32) -> NaiveDateTime {
        DateTime::from_timestamp((timestamp as f32 * timestamp_factor) as i64, 0)
            .unwrap()
            .naive_utc()
    }

    fn time_to_timestamp(time: &NaiveDateTime, timestamp_factor: f32) -> i32 {
        (time.and_utc().timestamp() as f32 / timestamp_factor) as i32
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
    ) -> Result<Option<ndarray::Array1<i32>>, hdf5::Error> {
        let date = time.date();

        // Try to find the index for the specified date.
        match self.days_map.get(&date).copied() {
            Some(date_index) => {
                // Date exists, perform query within its range.
                self.query_in_range(time, direction, bound, date_index)
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
    pub fn query_after(
        &self,
        time: &NaiveDateTime,
    ) -> Result<Option<ndarray::Array1<i32>>, hdf5::Error> {
        self.query(time, QueryDirection::Forward, TimeQueryBound::Exclusive)
    }

    /// Convenience method to get the last value before or at the specified
    /// time. Returns None if no such value exists, or an HDF5 error if
    /// there is a problem accessing the data.
    pub fn query_before(
        &self,
        time: &NaiveDateTime,
    ) -> Result<Option<ndarray::Array1<i32>>, hdf5::Error> {
        self.query(time, QueryDirection::Backward, TimeQueryBound::Inclusive)
    }

    /// Helper function to query within a known date range.
    fn query_in_range(
        &self,
        time: &NaiveDateTime,
        direction: QueryDirection,
        bound: TimeQueryBound,
        date_index: usize,
    ) -> Result<Option<ndarray::Array1<i32>>, hdf5::Error> {
        let date = time.date();

        let next_day_index = self
            .days_map
            .lower_bound(Bound::Excluded(&date))
            .next()
            .map(|(_, &i)| i)
            .unwrap_or(self.data.nrows());

        let start_index = date_index.saturating_sub(1);

        // TODO Is this min necessary?
        let end_index = (next_day_index + 1).min(self.data.nrows());

        let search_range = self.data.slice(s![start_index..end_index, ..]);
        let timestamp = Self::time_to_timestamp(time, self.timestamp_factor);

        let row_index = match (direction, bound) {
            (QueryDirection::Forward, TimeQueryBound::Inclusive) => search_range
                .column(0)
                .to_vec()
                .partition_point(|ts| ts < &timestamp),
            (QueryDirection::Forward, TimeQueryBound::Exclusive) => search_range
                .column(0)
                .to_vec()
                .partition_point(|ts| ts <= &timestamp),
            (QueryDirection::Backward, TimeQueryBound::Inclusive) => search_range
                .column(0)
                .to_vec()
                .partition_point(|ts| ts <= &timestamp)
                .saturating_sub(1),
            (QueryDirection::Backward, TimeQueryBound::Exclusive) => search_range
                .column(0)
                .to_vec()
                .partition_point(|ts| ts < &timestamp)
                .saturating_sub(1),
        };

        if row_index >= search_range.nrows() {
            return Ok(None);
        }

        Ok(Some(search_range.row(row_index).to_owned()))
    }

    /// Helper function to handle cases where the specified date does not exist.
    fn query_nearest_day(
        &self,
        date: NaiveDate,
        direction: QueryDirection,
    ) -> Result<Option<ndarray::Array1<i32>>, hdf5::Error> {
        match direction {
            QueryDirection::Backward => {
                // Fallback to last row of previous day

                if let Some((_, prev_day_index)) =
                    self.days_map.upper_bound(Bound::Excluded(&date)).prev()
                {
                    let next_day_index = self
                        .days_map
                        .lower_bound(Bound::Excluded(&date))
                        .next()
                        .map(|(_, &i)| i)
                        .unwrap_or(self.data.nrows());

                    let search_range = self.data.slice(s![*prev_day_index..next_day_index, ..]);
                    if !search_range.is_empty() {
                        return Ok(Some(search_range.row(search_range.nrows() - 1).to_owned()));
                    }
                }
            }
            QueryDirection::Forward => {
                // Fallback to first row of next day
                if let Some((_, next_day_index)) =
                    self.days_map.lower_bound(Bound::Excluded(&date)).next()
                {
                    let next_next_day_index = self
                        .days_map
                        .lower_bound(Bound::Excluded(&self.index_to_date(*next_day_index)))
                        .next()
                        .map(|(_, &i)| i)
                        .unwrap_or(self.data.nrows());

                    let search_range = self
                        .data
                        .slice(s![*next_day_index..next_next_day_index, ..]);
                    if !search_range.is_empty() {
                        return Ok(Some(search_range.row(0).to_owned()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Helper to get the date corresponding to a row index
    fn index_to_date(&self, index: usize) -> NaiveDate {
        // This assumes days_map is sorted and maps to the first row of each day
        // So we reverse search for the largest index <= given index
        self.days_map
            .iter()
            .rev()
            .find(|(_, &i)| i <= index)
            .map(|(d, _)| *d)
            .expect("Index should always map to a date")
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
